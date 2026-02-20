using MyApp.Models;

namespace MyApp.Services
{
    public class UserService
    {
        private readonly IRepository _repo;

        public UserService(IRepository repo)
        {
            _repo = repo;
        }

        public async Task<User> GetUser(int id)
        {
            return await _repo.FindById(id);
        }

        public void DeleteUser(int id)
        {
            _repo.Delete(id);
        }
    }

    public interface IRepository
    {
        Task<User> FindById(int id);
        void Delete(int id);
    }

    public enum UserRole
    {
        Admin,
        User,
        Guest,
    }

    public struct Point
    {
        public int X;
        public int Y;
    }

    public static class Constants
    {
        public const int MaxRetries = 3;
    }
}
